//! Tests of the weight's notices against its limits.

use super::*;

/// The weight's two notices (`PhysicalBase.IsEncumbered`,
/// `UpdateInternalParameters`): none with nothing carried, encumbered once
/// any share of the weight is over nothing, over-encumbered at a whole
/// share of the base limits, and the health's two effects with them.
#[test]
fn the_weight_s_notices_follow_its_limits() {
    let Some(mut m) = movement(0.0) else { return };
    let db = crate::fixture::database().expect("database");
    let install = crate::fixture::install().expect("install");
    let hard = trp_data::setup::hard_settings::HardSettings::load(&install).expect("settings object");
    assert_eq!(m.encumbered(), (false, false), "nothing carried");
    // The first weight at which each notice shows, a kilogram at a time.
    let (mut first, mut over) = (None, None);
    for kg in 1..=120 {
        m.set_weight(&hard, &db.config, kg as f32).expect("reweighed");
        let (encumbered, whole) = m.encumbered();
        assert!(encumbered || !whole, "over-encumbered is encumbered too, at {kg} kg");
        assert_eq!(whole, m.settings.overweight >= 1.0, "at {kg} kg");
        if encumbered && first.is_none() {
            first = Some(kg);
        }
        if whole && over.is_none() {
            over = Some(kg);
        }
    }
    let (first, over) = (first.expect("encumbered below 120 kg"), over.expect("over-encumbered below 120 kg"));
    println!("  encumbered from {first} kg, over-encumbered from {over} kg");
    assert!(first < over, "{first} then {over}");
    // And the health's two effects with them.
    use trp_sim::condition::health::{Health, HealthSettings, Kind};
    let mut health = Health::new(std::sync::Arc::new(HealthSettings::new(&db.config).expect("config.Health")), 1);
    let shown = |health: &Health, kind: Kind| health.effects.iter().any(|effect| effect.kind == kind);
    health.set_encumbered(true, false);
    assert!(shown(&health, Kind::Encumbered) && !shown(&health, Kind::OverEncumbered));
    health.set_encumbered(true, true);
    assert!(shown(&health, Kind::Encumbered) && shown(&health, Kind::OverEncumbered));
    health.set_encumbered(false, false);
    assert!(!shown(&health, Kind::Encumbered) && !shown(&health, Kind::OverEncumbered));
}
