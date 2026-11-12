//! Tests of what a bullet's hit shows, beside the recorded ballistics courses.
//! The recordings are `courses3`'s; after the ballistics spec, "What a hit shows and sounds",
//! and the particles spec, "The modules' own draws". These pass trivially, with a note, where no
//! install is set up.

use std::sync::Arc;

use trp_data::drawing::particles::EmitterKind;
use trp_data::unity::Assets;
use trp_sim::effects::particles::{Particle, ParticleSystem};

/// Concrete's chips (`Drop Concrete`, the `Concrete` entry's fourth older
/// emitter) as `2-surfaces` recorded them: each particle's seed and its
/// spin, degrees a second, to the recorder's six figures.
const RECORDED: [(u32, f32); 8] = [
    (2_726_439_373, -263.636),
    (4_293_138_084, -88.1133),
    (206_744_624, 160.074),
    (564_404_918, -511.061),
    (3_989_034_624, 357.314),
    (229_475_408, 461.882),
    (3_221_379_546, 421.67),
    (3_641_401_921, -223.613),
];

/// Each chip spins by its seed as the engine draws it: the rotation module's
/// two constants at the draw of Unity's generator seeded with the seed plus
/// the module's constant, turned the other way where the flip's own draw
/// falls under the system's share (where the stand-in hash before it was
/// a median 2.9 degrees a frame off).
#[test]
fn a_concrete_chip_spins_as_the_engine_draws_it() {
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let mut assets = Assets::new(install).expect("manifest");
    let entry = trp_data::drawing::particles::effect_entry(&mut assets, "Concrete").expect("Concrete's entry");
    let data = entry
        .emitters
        .iter()
        .find_map(|e| match &e.kind {
            EmitterKind::System(a) if a.data.name == "Drop Concrete" => Some(a.data.clone()),
            _ => None,
        })
        .expect("Drop Concrete");
    let data = Arc::new(data);
    let dt = 0.001;
    for (seed, spin) in RECORDED {
        let mut system = ParticleSystem::new(data.clone());
        system.particles.push(Particle { position: [0.0; 3], velocity: [0.0; 3], size: 0.1, color: [1.0; 4], rotation: 0.0, lifetime: 1.0, age: 0.1, seed, trail: Vec::new() });
        system.step(dt);
        let turned = system.particles[0].rotation;
        let turned = if turned > 180.0 { turned - 360.0 } else { turned };
        let ours = turned / dt;
        assert!((ours - spin).abs() < 0.05, "seed {seed}: ours {ours} degrees a second, recorded {spin}");
    }
}
