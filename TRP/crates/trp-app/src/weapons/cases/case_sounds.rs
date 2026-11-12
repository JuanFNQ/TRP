//! Case sounds read and played, and cases let go and launched.

use super::*;

/// Starts reading the cases' sounds away from the frame, if the options'
/// `SPENT CASES` is on.
pub fn start_reading(app: &mut App, install: trp_data::Install) {
    let mut cases = app.world_mut().resource_mut::<Cases>();
    if cases.settings.is_none() {
        return;
    }
    let (send, receive) = channel();
    std::thread::spawn(move || match trp_data::audio::sound::shell_sounds(&install, true) {
        Ok(sounds) => {
            let _ = send.send(sounds);
        }
        Err(e) => eprintln!("the spent cases' sounds could not be read; they are silent: {e:#}"),
    });
    cases.coming = Some(Mutex::new(receive));
}

/// Takes the cases' sounds in once they have come.
pub(super) fn take_sounds(cases: &mut Cases) {
    let Some(coming) = cases.coming.take() else { return };
    let got = coming.lock().ok().and_then(|receive| match receive.try_recv() {
        Ok(sounds) => Some(Some(sounds)),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(None),
    });
    match got {
        None => cases.coming = Some(coming),
        Some(None) => {}
        Some(Some(mut read)) => {
            let clips: Vec<Option<Samples>> = std::mem::take(&mut read.clips).into_iter().map(|(_, clip)| clip.map(Samples::of_owned)).collect();
            info!("the spent cases' sounds: {} banks, {} clips read", read.banks.len(), clips.iter().flatten().count());
            let shuffles = vec![Shuffle::default(); read.banks.len()];
            cases.sounds = Some(CaseSounds { read, clips, shuffles, rng: Rng::new(0x5348_454c) });
        }
    }
}

/// A case's sound at a bounce (`PlayShellSound`, `BetterAudio.PlayAtPoint`):
/// the bank of its calibre's class and the surface met, none past the
/// bank's rolloff from the camera; its clips chosen by that distance, its
/// volume and pitch drawn (`RandomVolume`, `RandomPitch`), from a quarter
/// metre over the case, falling as one over the distance past 1 m and
/// panned at equal power (`PROVISIONAL`, as the steps').
pub(super) fn play_bounce(sounds: &mut CaseSounds, caliber: u8, surface: u8, place: [f32; 3], camera: &GlobalTransform, mixer: &Mixer, overall: f32) {
    let Some(at) = trp_sim::combat::shells::sound_surface(surface).and_then(|kind| sounds.read.bank(caliber, kind)) else { return };
    let bank = &sounds.read.banks[at];
    let case = convert::position(place);
    let distance = (case - camera.translation()).length();
    if distance > bank.rolloff {
        return;
    }
    let choice = choose(bank, &mut sounds.shuffles[at], distance, &mut sounds.rng);
    let drawn = if bank.delta_volume > 0.0 { bank.base_volume + sounds.rng.range(-bank.delta_volume, 0.0) } else { bank.base_volume };
    let to_source = case + Vec3::Y * SOUND_OVER - camera.translation();
    let heard = to_source.length();
    let falloff = if heard > FULL_WITHIN { FULL_WITHIN / heard } else { 1.0 };
    let side = if heard > 1e-4 { (to_source / heard).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
    // A source's volume is 0 to 1.
    let volume = drawn.clamp(0.0, 1.0) * overall * falloff;
    let pitch = Arc::new(AtomicU32::new(choice.pitch.to_bits()));
    let both = choice.clips.iter().all(Option::is_some);
    for (n, clip) in choice.clips.iter().enumerate() {
        let Some(samples) = clip.and_then(|clip| sounds.clips.get(clip)?.as_ref()) else { continue };
        let share = if !both {
            1.0
        } else if n == 0 {
            choice.share
        } else {
            1.0 - choice.share
        };
        let gain = volume * share;
        mixer.0.add_to(crate::audio::ui_sound::Group::CommonSounds, Voice::placed(samples, [angle.cos() * gain, angle.sin() * gain], pitch.clone()));
    }
}

/// A draw from 0 to 1 (the app's own: the game's draws are its engine's).
pub(super) fn draw(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed >> 8) as f32 / (1u32 << 24) as f32
}

/// `Random.Range` between two figures, either way about.
fn between(range: [f32; 2], seed: &mut u32) -> f32 {
    range[0] + (range[1] - range[0]) * draw(seed)
}

/// The round a shot of the weapon in hand fires: its chamber's, else its
/// magazine's top, else the weapon's own default.
pub(crate) fn fired_round(carried: &Carried, held: &HandsHeld, db: &trp_data::Database) -> Option<String> {
    let weapon = held.item.and_then(|id| carried.find(id))?;
    if let Some(round) = trp_sim::items::inventory::chambered(weapon) {
        return Some(round.kind.template.clone());
    }
    let magazine = weapon.slots.iter().zip(&weapon.kind.slots).find(|(_, slot)| slot.name == trp_sim::items::inventory::MAGAZINE_SLOT).and_then(|(part, _)| part.as_ref());
    if let Some(kind) = magazine.and_then(|magazine| trp_sim::items::inventory::magazine_round_at(magazine, 0)) {
        return Some(kind.template.clone());
    }
    db.items.get(&weapon.kind.template).and_then(|item| item.props.get("defAmmo")).and_then(|ammo| ammo.as_str()).map(str::to_owned)
}

/// The quaternion product, `b` first.
pub(super) fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    trp_sim::base::math::quat_mul(a, b)
}

/// A way turned by a quaternion.
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let r = Quat::from_xyzw(q[0], q[1], q[2], q[3]) * Vec3::from(v);
    [r.x, r.y, r.z]
}

/// A port's place and turn in the game's axes, as the rig would have it
/// unsqueezed: the rig's root kept, its view's squeeze taken out.
pub(super) fn unsqueezed(helper: &TransformHelper, root: Entity, entity: Entity) -> Option<([f32; 3], [f32; 4])> {
    let root = helper.compute_global_transform(root).ok()?;
    let (_, turn, at) = root.to_scale_rotation_translation();
    let now = helper.compute_global_transform(entity).ok()?;
    let (_, turn_now, at_now) = (Affine3A::from_rotation_translation(turn, at) * root.affine().inverse() * now.affine()).to_scale_rotation_translation();
    Some(([at_now.x, at_now.y, -at_now.z], convert::rotation_back(turn_now)))
}

/// Where a case is let go of from its port, and at what size
/// (`SpawnShell` waits for the end of the frame, then `transform.parent =
/// null`). The game's hands run their clips, and so the `ShellEject` mark
/// that starts that wait, before the first person's squeeze is put on
/// (`Player.LateUpdate`: `StartFovCoroutine`), and the squeeze waits for
/// the end of the frame too before it is taken off; the case's wait,
/// begun first, is taken as ended first (`PROVISIONAL`: Unity's order of
/// its end-of-frame waits, as read), so the case is let go of where the
/// port is drawn, the squeeze on, and keeps the scale that gives it
/// (`lossyScale`: its world matrix in its own axes, the diagonal; a case
/// hangs at its port at size one, `ParentShellToTransform`). `drawn` is
/// the port's world transform as drawn; `turn` the case's own as it hangs
/// there, in Bevy's axes (Unity's rotation takes no scale).
fn let_go(drawn: &GlobalTransform, turn: Quat) -> ([f32; 3], Vec3) {
    let affine = drawn.affine();
    let hung = Mat3::from(affine.matrix3) * Mat3::from_quat(convert::rotation(trp_sim::base::math::quat_euler([90.0, 0.0, 0.0])));
    let own = Mat3::from_quat(turn).transpose() * hung;
    let at = affine.translation;
    ([at.x, at.y, -at.z], Vec3::new(own.x_axis.x, own.y_axis.y, own.z_axis.z))
}

/// A case thrown (`SpawnExtractingCartridge`): from where its port has
/// carried it (`drawn`, the port as drawn: `let_go`), turned as it hangs
/// there (a quarter turn about x from the port's turn `port_turn`), with
/// the weapon's force and turn and the velocity it takes besides
/// (`parent`: the player's share). The move in the force is the weapon's
/// first port's (`port`; `first`, its turn and its place in its parent)
/// from where that was as the weapon was made, and the forward the case
/// flies by is that port's (`_shellPortsTransforms[0]`).
#[allow(clippy::too_many_arguments)]
pub(super) fn launch(commands: &mut Commands, cases: &mut Cases, settings: ShellSettings, range: &Range, ready: &crate::world::loose::Ready, (at, port_turn): ([f32; 3], [f32; 4]), drawn: Option<GlobalTransform>, first: ([f32; 4], Vec3), port: &crate::weapons::weapon::ShellPort, parent: [f32; 3]) {
    let case_turn = mul(port_turn, trp_sim::base::math::quat_euler([90.0, 0.0, 0.0]));
    let unsqueezed = at;
    let (at, scale) = drawn.map_or((at, Vec3::ONE), |drawn| let_go(&drawn, convert::rotation(case_turn)));
    debug!("a case let go of where its port is drawn, {:.3} m from the port unsqueezed, at a scale of {scale:.3}", Vec3::from(at).distance(Vec3::from(unsqueezed)));
    let (first_turn, local) = first;
    let moved = [local.x - port.rest[0], local.y - port.rest[1], -local.z - port.rest[2]];
    let e = port.extraction;
    let seed = &mut cases.seed;
    let force: [f32; 3] = std::array::from_fn(|k| moved[k] * e.multiplier + between(e.force[k], seed));
    let thrown = rotate(case_turn, force);
    let velocity: [f32; 3] = std::array::from_fn(|k| thrown[k] + parent[k]);
    let spin: [f32; 3] = std::array::from_fn(|k| between(e.turn[k], seed));
    let forward = rotate(first_turn, [0.0, 0.0, 1.0]);
    let cast = |from: [f32; 3], way: [f32; 3], reach: f32| range.cast(from, way, reach);
    let mut random = || draw(seed);
    let flight = Flight::new(at, velocity, spin, forward, settings, &cast, &mut random);
    let entity = spawn_case(commands, ready, Transform { translation: convert::position(at), rotation: convert::rotation(case_turn), scale });
    cases.flying.push(Flying { entity, flight, turn: case_turn, since: 0.0, caliber: ready.caliber });
}
