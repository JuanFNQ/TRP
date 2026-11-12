//! Tests of the cut under headgear: the templates, their choice by what is worn and the recorded fade.
use super::*;
use crate::items::inventory::Kind;
use std::sync::Arc;
use trp_data::character::inventory::Holder;

/// The install's fades (SPT's `settings.json`: 0.7 s, in by
/// `InverseExponential`, out by `EaseIn`).
fn settings() -> HeadphonesSettings {
    HeadphonesSettings { fade_duration: 0.7, fade_in: FadeType::InverseExponential, fade_out: FadeType::EaseIn }
}

/// The recording `courses7\10-anything\anything-212919` (recorder 0.14.0,
/// its mixer records of `GunsVolume` and `AmbientVolume`, equal on every
/// frame): the ZSh-1-2M put on at frame 414 and taken off at 717, each
/// frame's `dt` (s) and the value recorded on it (dB), to the frame each
/// fade ends on.
const PUT_ON: [(f32, f32); 84] = [
    (0.0083331, -0.0070857713), (0.0135764, -0.0489822663), (0.0083336, -0.09333112), (0.0083333, -0.151850864), (0.0094429, -0.235291153), (0.0083332, -0.324041247),
    (0.0083339, -0.426972419), (0.0083336, -0.544073462), (0.0083338, -0.6753511), (0.008333, -0.820788), (0.0083335, -0.980406), (0.0083336, -1.154199),
    (0.0083334, -1.34216082), (0.0083336, -1.5443), (0.0083336, -1.76061261), (0.0083332, -1.9910872), (0.0083336, -2.23574519), (0.0083335, -2.494574),
    (0.0083335, -2.76757526), (0.0083334, -3.054746), (0.0083335, -3.35609269), (0.0083735, -3.67316127), (0.0083341, -4.002947), (0.0083328, -4.3468523),
    (0.0083349, -4.70502043), (0.0083318, -5.07722473), (0.0083342, -5.46370935), (0.008333, -5.864311), (0.0083343, -6.279149), (0.0083331, -6.70810032),
    (0.0083334, -7.15124035), (0.0083337, -7.60856962), (0.0083329, -8.080026), (0.0083339, -8.56571), (0.0083333, -9.065535), (0.0083338, -9.579561),
    (0.0083333, -10.1077309), (0.0083336, -10.6500912), (0.0083336, -11.206625), (0.0083335, -11.7773247), (0.0083331, -12.3621693), (0.0083339, -12.9612446),
    (0.0083332, -13.574439), (0.0083336, -14.2018375), (0.0083335, -14.8434048), (0.0083336, -15.4991493), (0.0083339, -16.16909), (0.0083329, -16.8531265),
    (0.0083335, -17.551384), (0.0083338, -18.26384), (0.0083339, -18.9904747), (0.0083333, -19.7312336), (0.0083334, -20.4861717), (0.0083333, -21.2552738),
    (0.0083338, -22.0385971), (0.0083331, -22.8360252), (0.0083335, -23.6476612), (0.0083339, -24.4735165), (0.0083336, -25.31351), (0.0083334, -26.1676579),
    (0.0083334, -27.0359783), (0.0083333, -27.9184666), (0.0083338, -28.8151779), (0.0083331, -29.72598), (0.0083333, -30.6509819), (0.0083341, -31.59025),
    (0.0083332, -32.5435829), (0.0083339, -33.5111732), (0.0083335, -34.49289), (0.008333, -35.48872), (0.0085014, -36.51928), (0.008333, -37.54374),
    (0.0083334, -38.5824127), (0.0083412, -39.6362534), (0.0083274, -40.7025223), (0.0083334, -41.78372), (0.0083334, -42.8790855), (0.0083335, -43.9886436),
    (0.0085064, -45.1358452), (0.0083335, -46.27404), (0.0083328, -47.4263153), (0.0083339, -48.5929146), (0.0083332, -49.7735863), (0.0083336, -50.0),
];
const TAKEN_OFF: [(f32, f32); 84] = [
    (0.0083335, -46.03969), (0.0132698, -40.3707275), (0.0083336, -37.1730766), (0.0083336, -34.2287025), (0.0083334, -31.5176067), (0.0083339, -29.0211048),
    (0.0083331, -26.722559), (0.0083336, -24.6059418), (0.0083332, -22.6570663), (0.0083339, -20.8624), (0.0083331, -19.2100468), (0.0083335, -17.6884918),
    (0.0083336, -16.2874374), (0.0083332, -14.9974165), (0.0083338, -13.8094845), (0.0083333, -12.7157154), (0.0083338, -11.7085161), (0.0083331, -10.7811718),
    (0.0083338, -9.927207), (0.0083336, -9.1409), (0.0083329, -8.416936), (0.0083337, -7.750246), (0.0083333, -7.13639259), (0.0083337, -6.57113171),
    (0.0083335, -6.050658), (0.008334, -5.57138), (0.0083333, -5.130103), (0.0083331, -4.72378731), (0.0083344, -4.349595), (0.0083328, -4.0051074),
    (0.0083339, -3.68786454), (0.008333, -3.395778), (0.0083339, -3.1268), (0.0167577, -2.6487112), (0.008334, -2.43890572), (0.0083339, -2.24572134),
    (0.0083334, -2.067846), (0.008333, -1.90407038), (0.0083337, -1.75325274), (0.008334, -1.6143769), (0.0083333, -1.48651), (0.0083332, -1.368773),
    (0.0083338, -1.26035511), (0.0083332, -1.16052926), (0.0083334, -1.06861), (0.0083339, -0.9839654), (0.0083334, -0.906029344), (0.0083336, -0.834265351),
    (0.0083336, -0.768187642), (0.0083332, -0.7073432), (0.0083336, -0.651317835), (0.0083331, -0.59973), (0.0083339, -0.5522251), (0.0083337, -0.508484244),
    (0.0083334, -0.4682094), (0.0083332, -0.431126356), (0.0083334, -0.396978855), (0.0083332, -0.3655374), (0.0083342, -0.336581469), (0.0083331, -0.3099233),
    (0.008334, -0.285375118), (0.008333, -0.262773037), (0.0083337, -0.2419591), (0.0083339, -0.222793221), (0.008333, -0.205147266), (0.0083338, -0.18889904),
    (0.0083335, -0.1739353), (0.0083331, -0.160160661), (0.008334, -0.147473812), (0.0083336, -0.1357913), (0.0083331, -0.125038624), (0.0083336, -0.115132332),
    (0.0083337, -0.106012821), (0.0083333, -0.09761751), (0.008334, -0.0898838043), (0.008333, -0.08276701), (0.0083338, -0.0762105), (0.0083337, -0.07017255),
    (0.0083331, -0.0646144152), (0.0083337, -0.0594973564), (0.0083332, -0.05478561), (0.0083336, -0.05044639), (0.0083333, -0.048828125), (0.0083335, 0.0),
];

/// The recorded fade, frame by frame from the recorded `dt`s: the
/// default template applied as the raid began, then the ZSh-1-2M's
/// (`StrongMute`) on frame 414, the dry level down to -50 dB by `EaseIn`
/// (below the last asked) in 0.698 s; taken off on frame 717, back to 0
/// by `InverseExponential`, reaching -50/1024 on the frame its time is up
/// and 0 exactly on the next. Within 1e-5 dB of the recorded on every one
/// of the 168 frames (the largest gap 1.9e-6 dB, single precision's).
#[test]
fn the_recorded_fade_is_reproduced_from_the_frames_dts() {
    let mut phones = Headphones::new(settings(), Headphones::default_start());
    phones.apply(&DEFAULT);
    for _ in 0..200 {
        phones.step(0.008333);
    }
    assert!(!phones.fading());
    let mut worst: f32 = 0.0;
    phones.apply(&STRONG_MUTE);
    for (frame, (dt, recorded)) in PUT_ON.into_iter().enumerate() {
        phones.step(dt);
        for param in [Param::GunsVolume, Param::AmbientVolume] {
            let gap = (phones.value(param) - recorded).abs();
            assert!(gap < 1e-5, "on, frame {}: {:?} {} against {recorded}", 414 + frame, param, phones.value(param));
            worst = worst.max(gap);
        }
    }
    // The frame after the last: the target exactly, the fade done.
    phones.step(0.008333);
    assert_eq!(phones.value(Param::GunsVolume), -50.0);
    assert!(!phones.fading());
    phones.apply(&DEFAULT);
    for (frame, (dt, recorded)) in TAKEN_OFF.into_iter().enumerate() {
        phones.step(dt);
        let gap = (phones.value(Param::GunsVolume) - recorded).abs();
        assert!(gap < 1e-5, "off, frame {}: {} against {recorded}", 717 + frame, phones.value(Param::GunsVolume));
        worst = worst.max(gap);
    }
    assert_eq!(phones.value(Param::GunsVolume), 0.0, "frame 800: 0 exactly");
    println!("the recorded fade: the largest gap {worst:.2e} dB over 168 frames");
}

/// The net level of each group with each template held (the spec's
/// table): the dry way 50 dB down, the sent way a few under 0.
#[test]
fn each_group_s_level_under_each_template() {
    let held = |template: &'static Template| {
        let mut phones = Headphones::new(settings(), Headphones::default_start());
        phones.apply(template);
        for _ in 0..100 {
            phones.step(0.01);
        }
        phones
    };
    let close = |a: f32, b: f32| (a - b).abs() < 0.01;
    let high = held(&STRONG_MUTE);
    let low = held(&LOW_MUTE);
    let none = held(&DEFAULT);
    let table = [
        (Group::Guns, -4.95, -2.96),
        (Group::ClientPlayer, -5.95, -3.96),
        (Group::ObservedPlayer, -4.95, -2.96),
        (Group::Npc, -4.95, -2.96),
        (Group::CommonSounds, -4.95, -2.96),
        (Group::NatureSounds, -6.94, -4.95),
        (Group::TechnicalSounds, -6.94, -4.95),
        (Group::Ambient, -9.91, -4.95),
        (Group::Returns, -7.0, -5.0),
    ];
    for (group, under_high, under_low) in table {
        assert!(close(high.level(group), under_high), "{group:?} under High: {}", high.level(group));
        assert!(close(low.level(group), under_low), "{group:?} under Low: {}", low.level(group));
        assert!(none.level(group).abs() < 1e-6, "{group:?} with none: {}", none.level(group));
    }
    // A muffled voice seen from outside: its dry fader the dry level,
    // its fixed send through the bus, heard as the common sounds under
    // a helmet; with none its send reaches the bus at -80 dB, which
    // adds a thousandth of a decibel.
    let occluded = Group::SimpleOccluded;
    assert!(close(high.level(occluded), -4.95) && close(low.level(occluded), -2.96), "{} {}", high.level(occluded), low.level(occluded));
    assert!(none.level(occluded).abs() < 1e-3, "{}", none.level(occluded));
}

/// A tinnitus' dip is on the dry faders alone: with nothing worn the
/// world goes with it; under a High cut the sent way is left, about
/// 5 dB down; at its end the guns' group is back at the dry level.
#[test]
fn a_tinnitus_dips_the_dry_way_alone() {
    let mut phones = Headphones::new(settings(), Headphones::default_start());
    phones.apply(&DEFAULT);
    phones.step(1.0);
    phones.step(1.0);
    phones.ring(-80.0);
    assert!((phones.level(Group::Guns) + 80.0).abs() < 1e-3 && (phones.level(Group::ClientPlayer) + 80.0).abs() < 1e-3);
    phones.apply(&STRONG_MUTE);
    phones.step(1.0);
    phones.step(1.0);
    phones.ring(-80.0);
    assert!((phones.value(Param::GunsVolume) + 130.0).abs() < 1e-4, "the dry level plus the dip");
    assert!((phones.level(Group::Guns) + 5.0).abs() < 1e-3, "{}", phones.level(Group::Guns));
    assert!((phones.level(Group::ClientPlayer) + 6.0).abs() < 1e-3, "{}", phones.level(Group::ClientPlayer));
    phones.quiet();
    assert_eq!(phones.value(Param::GunsVolume), -50.0);
    assert!((phones.level(Group::Guns) + 4.95).abs() < 0.01);
}

/// An equipment of the three slots, and things of a kind.
fn equipment() -> Equipment {
    let slots = SLOTS.iter().map(|name| Holder { name: (*name).into(), ..Holder::default() }).collect();
    Equipment { item: Item::new(Arc::new(Kind { slots, ..Kind::default() })) }
}

fn thing(classes: &[&str], deaf: Deaf, armour: Option<(u32, f32)>, slots: &[&str]) -> Item {
    let (armor_class, durability) = armour.unwrap_or((0, 0.0));
    let slots = slots.iter().map(|name| Holder { name: (*name).into(), ..Holder::default() }).collect();
    Item::new(Arc::new(Kind { class_names: classes.iter().map(|class| (*class).to_owned()).collect(), deaf, armor_class, durability: [durability, durability], slots, ..Kind::default() }))
}

fn helmet(deaf: Deaf) -> Item {
    thing(&["helmet", "Headwear", "ArmoredEquipment"], deaf, None, &["mod_equipment_000", "mod_equipment_001"])
}

/// The order of the choice: a headset first, over a High helmet; the
/// helmet's own cut whatever its armour; an armoured part's; an
/// unarmoured part's never; the face cover's when the helmet has none.
#[test]
fn the_template_is_chosen_by_what_is_worn() {
    let mut worn = equipment();
    assert_eq!(choose(&worn), Choice::Deaf(Deaf::None));
    assert!(choose(&worn).template().is_default());
    worn.item.slots[1] = Some(helmet(Deaf::High));
    assert_eq!(choose(&worn).template().name, "StrongMute");
    worn.item.slots[0] = Some(thing(&["headset", "Headphones"], Deaf::None, None, &[]));
    assert_eq!(choose(&worn), Choice::Headset);
    assert!(choose(&worn).template().is_default(), "a headset: the default for now");
    worn.item.slots[0] = None;
    // A part: armoured, its Low counts; unarmoured, a High does not.
    let mut bare = helmet(Deaf::None);
    bare.slots[0] = Some(thing(&["ears", "ArmoredEquipment"], Deaf::High, None, &[]));
    worn.item.slots[1] = Some(bare.clone());
    assert_eq!(choose(&worn), Choice::Deaf(Deaf::None));
    bare.slots[1] = Some(thing(&["side armour", "ArmoredEquipment"], Deaf::Low, Some((3, 25.0)), &[]));
    worn.item.slots[1] = Some(bare);
    assert_eq!(choose(&worn), Choice::Deaf(Deaf::Low));
    // A headset fitted to the helmet wins too.
    let mut with_headset = helmet(Deaf::High);
    with_headset.slots[0] = Some(thing(&["rac", "Headphones"], Deaf::None, None, &[]));
    worn.item.slots[1] = Some(with_headset);
    assert_eq!(choose(&worn), Choice::Headset);
    // The face cover, armoured, under a helmet of none.
    worn.item.slots[1] = Some(helmet(Deaf::None));
    worn.item.slots[2] = Some(thing(&["welding mask", "FaceCover", "ArmoredEquipment"], Deaf::Low, Some((5, 40.0)), &[]));
    assert_eq!(choose(&worn), Choice::Deaf(Deaf::Low));
    worn.item.slots[2] = Some(thing(&["balaclava", "FaceCover", "ArmoredEquipment"], Deaf::High, None, &[]));
    assert_eq!(choose(&worn), Choice::Deaf(Deaf::None), "an unarmoured face cover's cut is never read");
}

/// The key changes with a thing worn or fitted, and is the same for the
/// same things.
#[test]
fn the_worn_key_follows_the_head_s_things() {
    let mut worn = equipment();
    let empty = worn_key(&worn);
    let helmet = helmet(Deaf::High);
    worn.item.slots[1] = Some(helmet.clone());
    let on = worn_key(&worn);
    assert_ne!(empty, on);
    assert_eq!(on, worn_key(&worn));
    let mut fitted = helmet;
    fitted.slots[0] = Some(thing(&["shield", "ArmoredEquipment"], Deaf::None, None, &[]));
    worn.item.slots[1] = Some(fitted);
    assert_ne!(on, worn_key(&worn));
    worn.item.slots[1] = None;
    assert_eq!(empty, worn_key(&worn));
}

/// The ambience's occluded send, 20 dB under its level for a cut, 5
/// for the default; the dry level on its four groups.
#[test]
fn the_configurator_s_values() {
    assert_eq!(STRONG_MUTE.value(Param::AmbientOccluded), -70.0);
    assert_eq!(DEFAULT.value(Param::AmbientOccluded), -5.0);
    for param in [Param::OcclusionVolume, Param::EnvironmentVolume, Param::GunsVolume, Param::OutEnvironmentVolume] {
        assert_eq!(LOW_MUTE.value(param), -50.0);
    }
    assert_eq!(Param::ALL.iter().map(|param| param.name()).collect::<std::collections::HashSet<_>>().len(), 17);
}
