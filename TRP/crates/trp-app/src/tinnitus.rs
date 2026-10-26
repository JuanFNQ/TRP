//! A contusion's tinnitus (health spec, "A contusion's tinnitus";
//! `Player.TryStartContusion`, `BetterAudio.StartTinnitusEffect`): as a
//! contusion is added to the character, unless headphones are in its
//! earpiece slot, the ears ring for the longer of 15 s and twice its time,
//! a second one setting a new end from then. Each frame, by the share of
//! that time gone: the ringing (`Sounds.TinnitusSound`, looping, unplaced,
//! on the master) at the hard settings' `TinnitusSound`, and the game's
//! `Guns` and `Main` groups (`ui_sound::Group`) at `MainChannelLevel` of
//! 2.5 times the share (`TinnitusEffectSpeedMultiplier`): the world's sound
//! gone a while and back. The tinnitus sends the game also sets name no
//! parameter of this install's mixer, and do nothing. Read as the run
//! begins with the options' `HEALTH SYSTEM` on.

use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::prelude::*;
use trp_data::curve::Curve;

use crate::inventory_view::Carried;
use crate::ui_sound::{no_cut, set_level, Cut, Group, Level, Levels, Mixer, Samples, Voice};

/// The least the ringing lasts (`StartTinnitusEffect`: the longer of this
/// and twice the contusion's time).
const LEAST: f32 = 15.0;
/// A level below which the app takes a group as silent.
const SILENT_DB: f32 = -80.0;

/// A contusion added to the character, with its working time
/// (`EffectAddedEvent`, `effect.WorkStateTime`).
#[derive(Message, Clone, Copy)]
pub struct Contused {
    pub time: f32,
}

/// The ringing once read, the curves it goes by, and the ringing under way
/// (its end and length on the game's clock, its own level and its cut).
#[derive(Resource)]
pub struct Tinnitus {
    clip: Option<Samples>,
    coming: Option<Mutex<Receiver<Samples>>>,
    ringing_curve: Curve,
    level_curve: Curve,
    speed: f32,
    end: f32,
    length: f32,
    own: Option<(Level, Cut)>,
}

/// Starts reading the ringing away from the frame, with the hard settings'
/// curves; none of it where they cannot be read.
pub fn start_reading(app: &mut App, install: trp_data::Install) {
    let hard = &app.world().resource::<crate::GameData>().hard;
    let figures = (|| -> anyhow::Result<(Curve, Curve, f32)> {
        Ok((hard.curve("TinnitusSound")?.clone(), hard.curve("MainChannelLevel")?.clone(), hard.float("TinnitusEffectSpeedMultiplier")?))
    })();
    let (ringing_curve, level_curve, speed) = match figures {
        Ok(figures) => figures,
        Err(e) => {
            warn!("the tinnitus' figures could not be read; a contusion makes none: {e:#}");
            return;
        }
    };
    let (send, receive) = channel();
    std::thread::spawn(move || match trp_data::sound::tinnitus_sound(&install) {
        Ok(clip) => {
            let _ = send.send(Samples::of_owned(clip));
        }
        Err(e) => eprintln!("the tinnitus' sound could not be read; it is silent: {e:#}"),
    });
    app.insert_resource(Tinnitus { clip: None, coming: Some(Mutex::new(receive)), ringing_curve, level_curve, speed, end: 0.0, length: 0.0, own: None });
}

/// Whether headphones are in the earpiece slot (`Headphones` there).
fn headphones_on(carried: &Carried) -> bool {
    let equipment = &carried.equipment.item;
    equipment.slots.iter().zip(&equipment.kind.slots).any(|(item, slot)| slot.name == "Earpiece" && item.as_ref().is_some_and(|item| item.kind.class_names.iter().any(|class| class == "Headphones")))
}

/// The frame's tinnitus: a contusion starts or lengthens it; while it rings,
/// its level and the groups' set; at its end, all put back.
pub fn ring(mut contused: MessageReader<Contused>, mut tinnitus: ResMut<Tinnitus>, carried: Res<Carried>, mixer: Option<Res<Mixer>>, levels: Option<Res<Levels>>, time: Res<Time>) {
    let tinnitus = &mut *tinnitus;
    if let Some(coming) = tinnitus.coming.take() {
        match coming.lock().ok().map(|receive| receive.try_recv()) {
            Some(Ok(clip)) => tinnitus.clip = Some(clip),
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => tinnitus.coming = Some(coming),
            _ => {}
        }
    }
    let Some(mixer) = mixer else {
        contused.clear();
        return;
    };
    let now = time.elapsed_secs();
    for added in contused.read() {
        if headphones_on(&carried) {
            info!("a contusion, and no tinnitus: headphones are worn");
            continue;
        }
        let ringing = tinnitus.end > now;
        tinnitus.length = LEAST.max(added.time * 2.0);
        tinnitus.end = now + tinnitus.length;
        if !ringing {
            // The ringing, looping on the master, at its own level.
            if let Some(clip) = tinnitus.clip.as_ref() {
                let own: Level = Default::default();
                set_level(&own, SILENT_DB);
                let cut = no_cut();
                let overall = levels.as_deref().map_or(1.0, Levels::overall_gain);
                mixer.0.add(Voice::whole(clip, overall).at_frame(mixer.0.clock(), true, cut.clone()).following_level(own.clone()));
                tinnitus.own = Some((own, cut));
            }
            info!("a contusion's tinnitus for {:.1} s", tinnitus.length);
        }
    }
    if tinnitus.length <= 0.0 {
        return;
    }
    let (guns, main) = (mixer.0.level(Group::Guns), mixer.0.level(Group::Main));
    if now < tinnitus.end {
        let share = 1.0 - (tinnitus.end - now) / tinnitus.length;
        let dip = tinnitus.level_curve.evaluate(tinnitus.speed * share);
        // PROVISIONAL: the main group's level in a raid taken as 0 dB (the game
        // sets it as the raid begins; not read); the groups' levels taken at
        // each block of the app's mix (64 frames, `ui_sound::BLOCK`).
        // The guns' at the headphones' dry level (none worn: 0 dB) and the
        // main's at its own (0 dB in a raid), each plus the dip.
        set_level(&guns, dip);
        set_level(&main, dip);
        if let Some((own, _)) = &tinnitus.own {
            let volume = tinnitus.ringing_curve.evaluate(share).clamp(0.0, 1.0);
            set_level(own, if volume > 1e-4 { 20.0 * volume.log10() } else { SILENT_DB });
        }
    } else {
        set_level(&guns, 0.0);
        set_level(&main, 0.0);
        if let Some((_, cut)) = tinnitus.own.take() {
            cut.store(mixer.0.clock(), std::sync::atomic::Ordering::Release);
        }
        tinnitus.length = 0.0;
        info!("the tinnitus is over");
    }
}
