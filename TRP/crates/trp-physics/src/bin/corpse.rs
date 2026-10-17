//! A recorded death set beside the engine's (`docs/spec/physics.md`): the
//! corpse of a trace of recorder 0.8.0 on (its first frame after the
//! death: every body where the game had it, still), rebuilt in PhysX from
//! the player prefab's bodies, colliders and joints (`trp_data::ragdoll`)
//! as the game builds it (`CorpseRagdoll`), stepped as the game steps it
//! (once a drawn frame, by the frames' average time), and each body's
//! place set beside the recorded one frame by frame.
//!
//! cargo run -p trp-physics --bin corpse -- <trace.jsonl> [--every <s>] [--conventions]
//!
//! With `--conventions` it tries every way of laying Unity's joint on
//! PhysX's (`JointConvention`) and with and without the mass scales, and
//! says how far each drifts in the first half second.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use trp_data::ragdoll::RagdollData;
use trp_physics::recorded::{drift, read, simulate};
use trp_physics::*;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().ok_or_else(|| anyhow!("a trace, please"))?;
    let every = args.iter().position(|a| a == "--every").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(0.1f32);
    let trace = read(path)?;
    let mut assets = trp_data::unity::Assets::new(trp_data::Install::locate()?)?;
    let data = RagdollData::load(&mut assets)?;
    println!(
        "{} frames after the death; the ground {:.3} m; the corpse's velocity {:?}; each step {:.5} s",
        trace.frames.len(),
        trace.ground,
        trace.velocity,
        trace.step
    );
    if args.iter().any(|a| a == "--angles") {
        // Each joint's twist about its axis, counted from the prefab's rest
        // and from the pose at death: at death, and the least and most the
        // recording reaches, against the prefab's limits.
        let rest: HashMap<&str, Q4> = data.bodies.iter().map(|b| (b.bone.as_str(), b.rest.1)).collect();
        let twist = |d: Q4, axis: V3| -> f32 {
            let d = if d[3] < 0.0 { [-d[0], -d[1], -d[2], -d[3]] } else { d };
            let a = { let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt(); [axis[0] / n, axis[1] / n, axis[2] / n] };
            2.0 * (d[0] * a[0] + d[1] * a[1] + d[2] * a[2]).atan2(d[3]).to_degrees()
        };
        for j in &data.joints {
            let (Some(own_rest), Some(con_rest)) = (rest.get(j.bone.as_str()), rest.get(j.connected.as_str())) else { continue };
            let rest_rel = mul(inverse(*con_rest), *own_rest);
            let rels: Vec<Q4> = trace.frames.iter().filter_map(|f| Some(mul(inverse(f.bodies.get(&j.connected)?.1), f.bodies.get(&j.bone)?.1))).collect();
            let Some(first) = rels.first().copied() else { continue };
            let from_rest: Vec<f32> = rels.iter().map(|r| twist(mul(inverse(rest_rel), *r), j.axis)).collect();
            let from_death: Vec<f32> = rels.iter().map(|r| twist(mul(inverse(first), *r), j.axis)).collect();
            let span = |v: &[f32]| v.iter().fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(*x), hi.max(*x)));
            let (r0, (rlo, rhi), (dlo, dhi)) = (from_rest[0], span(&from_rest), span(&from_death));
            println!(
                "{:>22} in {:>18}: limits {:>5}..{:<5} from rest: at death {:7.1}, reached {:7.1}..{:<7.1}  from death: {:7.1}..{:<7.1}",
                j.bone.trim_start_matches("Base Human"),
                j.connected.trim_start_matches("Base Human"),
                j.low_twist.limit,
                j.high_twist.limit,
                r0,
                rlo,
                rhi,
                dlo,
                dhi
            );
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--conventions") {
        for swings_swapped in [false, true] {
            for scales_inverted in [false, true] {
                let convention = JointConvention { own_first: true, twist_negated: false, swings_swapped, scales_inverted };
                let run = simulate(&data, &trace, convention, true);
                println!(
                    "swings swapped {swings_swapped:5}  scales inverted {scales_inverted:5}: drift {:.4} m to 0.25 s, {:.4} m to 0.5 s, {:.4} m to 1 s",
                    drift(&trace, &run, 0.25),
                    drift(&trace, &run, 0.5),
                    drift(&trace, &run, 1.0)
                );
            }
        }
        return Ok(());
    }
    let run = simulate(&data, &trace, JointConvention::GAME, true);
    let watched = ["Base HumanPelvis", "Base HumanHead", "Base HumanLCalf", "Base HumanRForearm1"];
    println!("{:>6}  {}", "dead_t", watched.iter().map(|w| format!("{:>30}", w.trim_start_matches("Base Human"))).collect::<Vec<_>>().join(""));
    let mut next = 0.0;
    for (frame, places) in trace.frames.iter().skip(1).zip(&run.places) {
        if frame.dead_t + 1e-6 < next {
            continue;
        }
        next = frame.dead_t + every;
        let cells: Vec<String> = watched
            .iter()
            .map(|name| match (frame.bodies.get(*name), places.get(*name)) {
                (Some((recorded, _)), Some(at)) => format!("{:>30}", format!("{:.3} {:.3} | {:.3} {:.3}", recorded[1], at[1], recorded[2], at[2])),
                _ => format!("{:>30}", "-"),
            })
            .collect();
        println!("{:6.3}  {}   (dt {:.4})", frame.dead_t, cells.join(""), frame.dt);
    }
    println!("drift: {:.4} m to 0.25 s, {:.4} m to 0.5 s, {:.4} m to 1 s (recorded y, simulated y | recorded z, simulated z above)", drift(&trace, &run, 0.25), drift(&trace, &run, 0.5), drift(&trace, &run, 1.0));
    Ok(())
}
