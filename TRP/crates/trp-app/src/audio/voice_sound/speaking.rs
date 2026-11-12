//! Phrases spoken in the chosen voice.

use super::*;

/// The frame's voice: the banks taken in when they come; the speaker told
/// what happened, in the game's order (the health's damage and effects,
/// then the hits, the breath, the face shield, its own update), and what
/// it says played.
#[allow(clippy::too_many_arguments)]
pub fn speak(
    mut voice: ResMut<VoiceSounds>,
    mut controller: ResMut<Controller>,
    carried: Res<crate::screens::inventory_view::Carried>,
    game: Res<crate::GameData>,
    mut received: MessageReader<crate::effects::contusion_view::Received>,
    mut again: MessageReader<crate::player::death::BeginAgain>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    time: Res<Time>,
    (mut phrases, mut told, mut voiced): (MessageReader<PhraseAsked>, MessageWriter<PhraseTold>, ResMut<Voiced>),
) {
    let voice = &mut *voice;
    if let Some(coming) = voice.coming.take() {
        match coming.lock().ok().map(|receive| receive.try_recv()) {
            Some(Ok(read)) => {
                let banks = read.voice.banks.iter().map(Bank::of).collect();
                voice.speaker.set_banks(banks);
                voiced.0 = voice.speaker.triggers().collect();
                voice.asked = read.voice.banks.iter().map(|b| b.clips.iter().any(|c| c.clip.is_some())).collect();
                voice.clips = read.voice.banks.into_iter().map(|b| b.clips.into_iter().map(|c| (c.clip.map(Samples::of_owned), c.volume)).collect()).collect();
                info!(
                    "the voice {} ({}): {} banks, {} clips; {}",
                    read.voice.name,
                    read.voice.bundle,
                    voice.clips.len(),
                    voice.clips.iter().flatten().filter(|(c, _)| c.is_some()).count(),
                    if read.crack.is_some() { "a bone's crack" } else { "no bone's crack" }
                );
                voice.crack = read.crack.map(Samples::of_owned);
                (voice.speech, voice.character) = (read.speech, read.character);
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => voice.coming = Some(coming),
            _ => {}
        }
    }
    let mut heard = std::mem::take(controller.voice_heard.get_or_insert_with(Vec::new));
    // Dead, the controller no longer takes what health did (`controller::step`):
    // the killing hit's damage and the death are taken here.
    if let Some(health) = controller.health.as_mut().filter(|health| !health.alive) {
        heard.extend(health.take_events());
    }
    let hits: Vec<_> = received.read().copied().collect();
    let now = time.elapsed_secs();
    let mut cracks = 0;
    // A run begun again: a new character, its speaker new.
    if again.read().count() > 0 {
        voice.speaker = std::mem::replace(&mut voice.speaker, Speaker::new(0)).renewed();
        if let Some(cut) = voice.playing.take() {
            if let Some(mixer) = mixer.as_deref() {
                cut.store(mixer.0.clock(), std::sync::atomic::Ordering::Release);
            }
        }
        voice.shield = None;
    }
    let Some(health) = controller.health.as_ref() else { return };
    if !voice.speaker.has_banks() {
        return;
    }
    let stamina = &controller.movement.stamina;
    let audible = !stamina.holding_breath() && ((voice.exhaustion_heard && stamina.legs.exhausted()) || stamina.oxygen.exhausted());
    let situation = |health: &trp_sim::condition::health::Health| Situation {
        alive: health.alive,
        status: Status::of(health),
        on_painkillers: controller.movement.conditions.on_painkillers,
        painkiller: health.active(Kind::PainKiller, None).is_some(),
        audible,
    };
    let s = situation(health);
    // The parts hit, first (`ApplyDamageInfo` sets `LastDamagedBodyPart`
    // before the damage is taken): what a death this frame goes by.
    for hit in &hits {
        voice.speaker.struck(hit.part);
    }
    for event in heard {
        match event {
            Event::Damaged(_, damage, kind) => voice.speaker.damaged(kind, damage, now, &s),
            Event::Started(kind, part) => {
                voice.speaker.effect_started(kind, part);
                if kind == Kind::Fracture {
                    cracks += 1;
                }
                if kind == Kind::Berserk {
                    voice.speaker.berserk(now, &s);
                }
            }
            Event::Removed(kind, part, _) => voice.speaker.effect_removed(kind, part),
            Event::Died(kind) => voice.speaker.died(kind, now, &s),
            _ => {}
        }
    }
    for hit in &hits {
        voice.speaker.hit(hit.part, hit.damage, hit.kind, now, &s);
    }
    // A shot of the character's own makes it aware (`OnMakingShot`).
    if controller.shots_made != voice.shots {
        if controller.shots_made > voice.shots {
            voice.speaker.aware(15.0, now);
        }
        voice.shots = controller.shots_made;
    }
    // The phrases asked for (`Player.Say`, demanded): said by the speaker's
    // rules, alive; told as they begin.
    for asked in phrases.read() {
        if s.alive && voice.speaker.say(asked.trigger, asked.aggressive, now, &s) {
            told.write(PhraseTold { trigger: asked.trigger, length: voice.speaker.playing_for(now).unwrap_or(0.0) });
        }
    }
    // A bank's sounds read: the phrase waiting for them plays, if not over.
    let arrived: Vec<trp_data::audio::sound::PhraseBank> = voice.decoded.as_ref().and_then(|d| d.lock().ok()).map(|d| d.try_iter().collect()).unwrap_or_default();
    for bank in arrived {
        let Some(at) = voice.speaker.triggers().position(|t| t == bank.trigger) else { continue };
        if let Some(slot) = voice.clips.get_mut(at) {
            *slot = bank.clips.into_iter().map(|c| (c.clip.map(Samples::of_owned), c.volume)).collect();
        }
        if let Some((phrase, until)) = voice.waiting.filter(|(phrase, _)| phrase.bank == at) {
            voice.waiting = None;
            if now < until {
                voice.speaker.replay(phrase);
            }
        }
    }
    if s.alive {
        let parts = trp_sim::condition::health::Part::ALL.map(|p| health.part(p).current);
        if parts.iter().zip(voice.parts).any(|(now, was)| (now - was).abs() >= 0.01) {
            voice.parts = parts;
            voice.speaker.breath(now, &s);
        }
        if audible != voice.audible {
            voice.audible = audible;
            voice.speaker.audible_changed(now, &s);
        }
        // The headwear's and the face cover's shields (`FaceShieldObserver`,
        // `FaceCoverObserver`): muffled while either is down over armour on
        // the jaw. PROVISIONAL: the game sets it from the one that changed.
        let equipment = &carried.equipment;
        let shield = ["Headwear", "FaceCover"].map(|slot| (trp_sim::effects::visor::shield_in(equipment, slot).is_some_and(|sh| sh.down), trp_sim::sound::speaker::muffles(&game.db, equipment, slot)));
        let state = (shield.iter().any(|(down, _)| *down), shield.iter().any(|(_, m)| *m));
        if voice.shield != Some(state) {
            let first = voice.shield.is_none();
            voice.shield = Some(state);
            if !first || state.1 {
                voice.speaker.face_shield(state.0, state.1, now, &s);
            }
        }
    }
    voice.speaker.update(now, &s);
    let Some(mixer) = mixer else { return };
    let overall = levels.as_deref().map_or(1.0, Levels::overall_gain);
    for said in voice.speaker.take_said() {
        if let Some(cut) = voice.playing.take() {
            cut.store(mixer.0.clock(), Ordering::Release);
        }
        let Said::Phrase(phrase) = said else { continue };
        // A bank whose sounds are not read yet: asked of the reading
        // thread, the phrase played when they come.
        if voice.asked.get(phrase.bank) == Some(&false) {
            voice.asked[phrase.bank] = true;
            let trigger = voice.speaker.triggers().nth(phrase.bank);
            if let (Some(ask), Some(trigger)) = (voice.ask.as_ref(), trigger) {
                let _ = ask.send(trigger);
            }
        }
        if voice.clips.get(phrase.bank).and_then(|b| b.get(phrase.clip)).is_some_and(|(samples, _)| samples.is_none()) {
            voice.waiting = Some((phrase, now + voice.speaker.playing_for(now).unwrap_or(0.0)));
            continue;
        }
        let Some((Some(samples), volume)) = voice.clips.get(phrase.bank).and_then(|b| b.get(phrase.clip)) else { continue };
        let base = voice.speech.as_ref().map_or(1.0, |p| p.overall_volume);
        let gain = overall * base * volume * mixer.0.speech_gain(phrase.muffled);
        let cut = no_cut();
        mixer.0.add_to(Group::ClientPlayer, Voice::whole(samples, gain).at_frame(mixer.0.clock(), false, cut.clone()));
        voice.playing = Some(cut);
        if voice.logged < 64 {
            voice.logged += 1;
            info!("the voice: bank {} clip {}{} at {gain:.3}", phrase.bank, phrase.clip, if phrase.muffled { ", muffled" } else { "" });
        }
    }
    // A bone's crack where the feet are, by the character's group's rolloff.
    if let (Some(crack), Ok(camera)) = (voice.crack.as_ref(), camera.single()) {
        for _ in 0..cracks {
            let at = convert::position(controller.movement.position);
            let to = at - camera.translation();
            let distance = to.length();
            if distance > CRACK_ROLLOFF {
                continue;
            }
            let falloff = voice.character.as_ref().map_or(1.0, |p| p.rolloff_gain(distance, CRACK_ROLLOFF).max(0.0) * p.overall_volume);
            let side = if distance > 1e-4 { (to / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
            let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
            let gain = overall * falloff;
            // The `Character` source group's: `CommonSounds`.
            mixer.0.add_to(Group::CommonSounds, Voice::placed(crack, [angle.cos() * gain, angle.sin() * gain], Arc::new(AtomicU32::new(1f32.to_bits()))));
            info!("a bone's crack, {distance:.1} m off");
        }
    }
}
