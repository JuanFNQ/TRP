//! A reload through the weapon's build made anew (`Firearm::carry_on_from`;
//! firing spec, "A build made anew in a reload"): the SCAR-L reloading a
//! PMAG Gen M3 40 from its stock 30-round, its build changed for the
//! 40-round's while no magazine is in sight, goes on as the same reload
//! left alone does, state for state, mark for mark, to its end. Those that
//! need the install pass trivially, with a note, where none is configured.

use trp_sim::weapon_animator::WeaponAnimatorData;
use trp_sim::{Firearm, FirearmEvent};

const SCAR_L: &str = "6184055050224f204c1da540";
/// The SCAR-L with the PMAG Gen M3 40 (`544a378f4bdc2d30388b4567`, index 3).
const SCAR_L_40: &str = "6184055050224f204c1da540[mod_magazine=544a378f4bdc2d30388b4567[]]";

#[test]
fn a_reload_goes_on_through_a_build_made_anew() {
    let Some(install) = crate::fixture::install() else {
        eprintln!("no install configured: skipped");
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");
    let stock = trp_data::weapon::assemble(&mut assets, db, SCAR_L).expect("the SCAR-L");
    let forty = trp_data::weapon::assemble_key(&mut assets, db, SCAR_L_40).expect("the SCAR-L with a 40-round");
    let (data, data_forty) = (WeaponAnimatorData::load(&assets, &stock).expect("its controller"), WeaponAnimatorData::load(&assets, &forty).expect("its controller"));
    let (settings, settings_forty) = (trp_sim::firearm_settings(&assets, db, &hard, &stock).expect("settings"), trp_sim::firearm_settings(&assets, db, &hard, &forty).expect("settings"));
    assert_eq!(settings_forty.magazine_variant, 3);

    const FRAME: f32 = 1.0 / 120.0;
    let mut alone = Firearm::new(settings, &data);
    alone.carried();
    alone.draw();
    for _ in 0..240 {
        alone.step(FRAME);
    }
    assert!(alone.is_idle());
    alone.next_magazine_variant(3);
    alone.reload_with(40);
    let mut swapped = alone.clone();
    let said = |events: Vec<FirearmEvent>| events.into_iter().filter(|e| !matches!(e, FirearmEvent::Sound(_))).collect::<Vec<_>>();
    let mut changed_at = None;
    for frame in 0..(10.0 / FRAME) as usize {
        if changed_at.is_none() && swapped.magazine_out_of_sight() {
            let mut anew = Firearm::new(settings_forty.clone(), &data_forty);
            assert!(anew.carry_on_from(&swapped), "the same machines");
            swapped = anew;
            changed_at = Some(frame);
        }
        let (a, b) = (said(alone.step(FRAME)), said(swapped.step(FRAME)));
        assert_eq!(a, b, "frame {frame}: what each said");
        let ((sa, ta, na), (sb, tb, nb)) = (alone.animator().state(), swapped.animator().state());
        assert_eq!((sa, na), (sb, nb), "frame {frame}: the state");
        assert!((ta - tb).abs() < 1e-5, "frame {frame}: {ta} against {tb}");
    }
    assert!(changed_at.is_some(), "the old magazine left the hand and the new one was not yet shown at some frame");
    assert!(swapped.is_idle() && !swapped.hands_busy(), "the reload is over");
    assert_eq!(swapped.magazine, Some(40));
    assert_eq!(swapped.animator().get("RelTypeOld"), 3.0, "the 40-round's clips from its MagIn on");
}

/// The weapon's motion goes on through the change too (`Hands::
/// carry_on_from`): the SCAR-L's hands, breathing for four seconds, taken
/// over by the 40-round build's: on the next step its weapon is where the
/// old build's own would have been, to within a hundredth of that step's
/// own motion; made afresh instead, it is off by more than that step's
/// motion.
#[test]
fn the_weapons_motion_goes_on_through_a_build_made_anew() {
    use trp_sim::{Hands, HandsInput, ViewSettings};
    let Some(install) = crate::fixture::install() else {
        eprintln!("no install configured: skipped");
        return;
    };
    let db = crate::fixture::database().expect("database");
    let hard = trp_data::hard_settings::HardSettings::load(&install).expect("settings object");
    let engine = trp_data::engine::EngineSettings::load(&install).expect("engine settings");
    let mut assets = trp_data::unity::Assets::new(install).expect("manifest");
    let data = trp_data::player::HandsData::load(&mut assets).expect("player prefab");
    let stock = trp_data::weapon::assemble(&mut assets, db, SCAR_L).expect("the SCAR-L");
    // (Its whole stock build, the 40-round for its magazine: the hands need
    // its barrel's muzzle.)
    let mut preset = db.default_preset(SCAR_L).expect("its preset").clone();
    for item in preset.items.iter_mut().filter(|item| item.slot.as_deref() == Some("mod_magazine")) {
        item.template = "544a378f4bdc2d30388b4567".to_owned();
    }
    let forty = trp_data::weapon::assemble_preset(&mut assets, db, &preset).expect("the SCAR-L with a 40-round");
    let hands_of = |weapon| Hands::new(trp_sim::hands_settings(data.clone(), db, &hard, &engine, weapon, ViewSettings::default()).expect("hands settings"));
    const DT: f32 = 1.0 / 60.0;
    let still = HandsInput { idle: true, pose_level: 1.0, smoothed_pose_level: 1.0, ..HandsInput::default() };
    let mut old = hands_of(&stock);
    for _ in 0..240 {
        old.advance(DT, &still);
    }
    let (mut carried, mut fresh) = (hands_of(&forty), hands_of(&forty));
    carried.carry_on_from(&old);
    let place = |hands: &Hands| -> Vec<f32> { hands.hands_position.current.iter().chain(&hands.hands_rotation.current).chain(&hands.camera_rotation.current).copied().collect() };
    let apart = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f32>().sqrt();
    let before = place(&old);
    for hands in [&mut old, &mut carried, &mut fresh] {
        hands.advance(DT, &still);
    }
    let moved = apart(&place(&old), &before);
    let (carried_off, fresh_off) = (apart(&place(&old), &place(&carried)), apart(&place(&old), &place(&fresh)));
    println!("a step's own motion {moved:.2e}; carried over {carried_off:.2e} from the old build's, made afresh {fresh_off:.2e}");
    assert!(moved > 0.0, "the weapon breathes");
    assert!(carried_off < moved * 0.01, "carried over: {carried_off} against a step's {moved}");
    assert!(fresh_off > moved, "made afresh it would jump: {fresh_off} against a step's {moved}");
}
